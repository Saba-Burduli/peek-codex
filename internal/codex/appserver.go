// Package codex contains the only code that knows the Codex app-server
// JSON-RPC protocol. Callers receive stable domain types only.
package codex

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"sync"
	"time"

	"github.com/Saba-Burduli/peek-codex/internal/domain"
)

const (
	PageSize              = 50
	DefaultRequestTimeout = 10 * time.Second
)

type SessionSource interface {
	ListSessions(context.Context, string) (domain.SessionPage, error)
	ReadThread(context.Context, domain.SessionID) (domain.Session, error)
	Close() error
}

type AppServerSource struct {
	command   *exec.Cmd
	stdin     io.WriteCloser
	lines     chan lineResult
	cancel    context.CancelFunc
	cancelled <-chan struct{}
	done      chan struct{}

	mu      sync.Mutex
	close   sync.Once
	nextID  uint64
	timeout time.Duration
}

type lineResult struct {
	line []byte
	err  error
}

func Start(ctx context.Context, program, version string) (*AppServerSource, error) {
	return start(ctx, program, version, DefaultRequestTimeout)
}

func start(parent context.Context, program, version string, timeout time.Duration) (*AppServerSource, error) {
	ctx, cancel := context.WithCancel(parent)
	command := exec.CommandContext(ctx, program, "app-server", "--stdio")
	command.Stderr = io.Discard
	stdin, err := command.StdinPipe()
	if err != nil {
		cancel()
		return nil, fmt.Errorf("app-server stdin was unavailable: %w", err)
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		cancel()
		return nil, fmt.Errorf("app-server stdout was unavailable: %w", err)
	}
	if err := command.Start(); err != nil {
		cancel()
		return nil, fmt.Errorf("could not start `codex app-server`: %w; install Codex CLI 0.146.0 or newer and ensure `codex` is on PATH", err)
	}

	source := &AppServerSource{
		command:   command,
		stdin:     stdin,
		lines:     make(chan lineResult, 1),
		cancel:    cancel,
		cancelled: ctx.Done(),
		done:      make(chan struct{}),
		nextID:    1,
		timeout:   timeout,
	}
	go source.readLines(stdout)
	if _, err := source.request(ctx, "initialize", map[string]any{
		"clientInfo": map[string]string{
			"name":    "peek-codex",
			"title":   "Peek Codex",
			"version": version,
		},
	}); err != nil {
		_ = source.Close()
		return nil, err
	}
	if err := source.notify("initialized", nil); err != nil {
		_ = source.Close()
		return nil, err
	}
	return source, nil
}

func (source *AppServerSource) ListSessions(ctx context.Context, cursor string) (domain.SessionPage, error) {
	result, err := source.request(ctx, "thread/list", map[string]any{
		"cursor":         nilIfEmpty(cursor),
		"limit":          PageSize,
		"sortKey":        "recency_at",
		"sortDirection":  "desc",
		"useStateDbOnly": true,
	})
	if err != nil {
		return domain.SessionPage{}, err
	}
	var response threadListResponse
	if err := json.Unmarshal(result, &response); err != nil {
		return domain.SessionPage{}, fmt.Errorf("could not decode thread/list response: %w", err)
	}
	sessions := make([]domain.Session, 0, len(response.Data))
	for _, thread := range response.Data {
		session, err := thread.session()
		if err != nil {
			return domain.SessionPage{}, err
		}
		sessions = append(sessions, session)
	}
	domain.SortSessions(sessions)
	return domain.SessionPage{Sessions: sessions, NextCursor: response.NextCursor}, nil
}

func (source *AppServerSource) ReadThread(ctx context.Context, id domain.SessionID) (domain.Session, error) {
	result, err := source.request(ctx, "thread/read", map[string]any{
		"threadId":     id.String(),
		"includeTurns": false,
	})
	if err != nil {
		return domain.Session{}, err
	}
	var response threadReadResponse
	if err := json.Unmarshal(result, &response); err != nil {
		return domain.Session{}, fmt.Errorf("could not decode thread/read response: %w", err)
	}
	return response.Thread.session()
}

func (source *AppServerSource) Close() error {
	var closeErr error
	source.close.Do(func() {
		source.cancel()
		_ = source.stdin.Close()
		closeErr = source.command.Wait()
		<-source.done
	})
	return closeErr
}

func (source *AppServerSource) notify(method string, params any) error {
	message := map[string]any{"jsonrpc": "2.0", "method": method}
	if params != nil {
		message["params"] = params
	}
	return source.write(message)
}

func (source *AppServerSource) request(parent context.Context, method string, params any) (json.RawMessage, error) {
	source.mu.Lock()
	defer source.mu.Unlock()
	ctx, cancel := context.WithTimeout(parent, source.timeout)
	defer cancel()
	id := source.nextID
	source.nextID++
	if err := source.write(map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params}); err != nil {
		return nil, err
	}
	for {
		select {
		case <-ctx.Done():
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				return nil, fmt.Errorf("app-server `%s` request timed out after %s", method, source.timeout)
			}
			return nil, errors.New("app-server request cancelled")
		case <-source.cancelled:
			return nil, errors.New("app-server request cancelled")
		case result := <-source.lines:
			if result.err != nil {
				return nil, result.err
			}
			response, err := decodeResponse(result.line, id)
			if err != nil {
				return nil, err
			}
			if response != nil {
				return response, nil
			}
		}
	}
}

func (source *AppServerSource) write(message any) error {
	encoded, err := json.Marshal(message)
	if err != nil {
		return fmt.Errorf("could not encode app-server request: %w", err)
	}
	encoded = append(encoded, '\n')
	if _, err := source.stdin.Write(encoded); err != nil {
		return fmt.Errorf("could not write app-server request: %w", err)
	}
	return nil
}

func (source *AppServerSource) readLines(stdout io.Reader) {
	defer close(source.done)
	reader := bufio.NewReader(stdout)
	for {
		line, err := reader.ReadBytes('\n')
		if len(bytes.TrimSpace(line)) > 0 {
			select {
			case source.lines <- lineResult{line: line}:
			case <-source.cancelled:
				return
			}
		}
		if err != nil {
			failure := lineResult{err: fmt.Errorf("could not read app-server response: %w", err)}
			if errors.Is(err, io.EOF) {
				failure.err = errors.New("app-server exited before replying; Codex CLI 0.146.0 or newer is required")
			}
			select {
			case source.lines <- failure:
			case <-source.cancelled:
			}
			return
		}
	}
}

func decodeResponse(line []byte, expectedID uint64) (json.RawMessage, error) {
	var response struct {
		ID     json.RawMessage `json:"id"`
		Result json.RawMessage `json:"result"`
		Error  *struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(line, &response); err != nil {
		return nil, fmt.Errorf("app-server returned invalid JSON: %w", err)
	}
	var id uint64
	if err := json.Unmarshal(response.ID, &id); err != nil || id != expectedID {
		return nil, nil
	}
	if response.Error != nil {
		if response.Error.Message == "" {
			return nil, errors.New("app-server returned a malformed JSON-RPC error")
		}
		return nil, terminalError("app-server request failed: %s", response.Error.Message)
	}
	if response.Result == nil {
		return nil, errors.New("app-server response did not contain a result")
	}
	return response.Result, nil
}

type threadListResponse struct {
	Data       []protocolThread `json:"data"`
	NextCursor string           `json:"nextCursor"`
}

type threadReadResponse struct {
	Thread protocolThread `json:"thread"`
}

type protocolThread struct {
	ID            string          `json:"id"`
	Name          *string         `json:"name"`
	Preview       string          `json:"preview"`
	CWD           string          `json:"cwd"`
	CreatedAt     int64           `json:"createdAt"`
	UpdatedAt     int64           `json:"updatedAt"`
	RecencyAt     *int64          `json:"recencyAt"`
	ModelProvider string          `json:"modelProvider"`
	Source        json.RawMessage `json:"source"`
	Status        struct {
		Type string `json:"type"`
	} `json:"status"`
	GitInfo *struct {
		Branch *string `json:"branch"`
	} `json:"gitInfo"`
}

func (thread protocolThread) session() (domain.Session, error) {
	id, err := domain.NewSessionID(thread.ID)
	if err != nil {
		return domain.Session{}, fmt.Errorf("invalid thread id: %w", err)
	}
	name := ""
	if thread.Name != nil {
		name = domain.SanitizeTerminalText(*thread.Name)
	}
	branch := ""
	if thread.GitInfo != nil && thread.GitInfo.Branch != nil {
		branch = domain.SanitizeTerminalText(*thread.GitInfo.Branch)
	}
	recency := thread.UpdatedAt
	if thread.RecencyAt != nil {
		recency = *thread.RecencyAt
	}
	provider := domain.SanitizeTerminalText(thread.ModelProvider)
	if source := displaySource(thread.Source); source != "" {
		provider += "/" + source
	}
	provider = domain.SanitizeTerminalText(provider)
	return domain.Session{
		ID: id, Name: name, Preview: domain.SanitizeTerminalText(thread.Preview),
		CWD: domain.SanitizeTerminalText(thread.CWD), CreatedAt: thread.CreatedAt,
		UpdatedAt: thread.UpdatedAt, RecencyAt: recency, Provider: provider,
		Status: domain.SanitizeTerminalText(thread.Status.Type), Branch: branch,
	}, nil
}

func terminalError(format string, values ...any) error {
	return errors.New(domain.SanitizeTerminalText(fmt.Sprintf(format, values...)))
}

func displaySource(raw json.RawMessage) string {
	var stringValue string
	if json.Unmarshal(raw, &stringValue) == nil {
		return domain.SanitizeTerminalText(stringValue)
	}
	var objectValue struct {
		Custom string `json:"custom"`
	}
	if json.Unmarshal(raw, &objectValue) == nil {
		return domain.SanitizeTerminalText(objectValue.Custom)
	}
	return ""
}

func nilIfEmpty(value string) any {
	if value == "" {
		return nil
	}
	return value
}
