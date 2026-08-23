package codex

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestAppServerHandshakePaginationAndRead(t *testing.T) {
	program, requests := fakeServer(t, `
read_line
reply '{"jsonrpc":"2.0","id":1,"result":{"userAgent":"fake"}}'
read_line
read_line
reply '{"jsonrpc":"2.0","id":2,"result":{"data":[{"id":"first","name":"Project health","preview":"must not be rendered","cwd":"/tmp/one","createdAt":1,"updatedAt":3,"recencyAt":3,"modelProvider":"openai","source":"cli","status":{"type":"notLoaded"},"gitInfo":null,"unknown":true}],"nextCursor":"next"}}'
read_line
reply '{"jsonrpc":"2.0","id":3,"result":{"data":[{"id":"second","name":null,"preview":"Second","cwd":"/tmp/two","createdAt":1,"updatedAt":2,"recencyAt":2,"modelProvider":"openai","source":{"custom":"vscode"},"status":{"type":"idle"},"gitInfo":{"branch":"main"}}],"nextCursor":null}}'
read_line
reply '{"jsonrpc":"2.0","id":4,"result":{"thread":{"id":"first","name":null,"preview":"First","cwd":"/tmp/one","createdAt":1,"updatedAt":3,"recencyAt":3,"modelProvider":"openai","source":"cli","status":{"type":"notLoaded"},"gitInfo":null}}}'
`)

	source, err := start(context.Background(), program, "test", time.Second)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = source.Close() })

	first, err := source.ListSessions(context.Background(), "")
	if err != nil {
		t.Fatal(err)
	}
	if first.Sessions[0].Title() != "Project health" || first.NextCursor != "next" {
		t.Fatalf("first page = %#v", first)
	}
	second, err := source.ListSessions(context.Background(), first.NextCursor)
	if err != nil {
		t.Fatal(err)
	}
	if second.Sessions[0].Branch != "main" || second.NextCursor != "" {
		t.Fatalf("second page = %#v", second)
	}
	detail, err := source.ReadThread(context.Background(), first.Sessions[0].ID)
	if err != nil || detail.ID != "first" {
		t.Fatalf("detail = %#v, err = %v", detail, err)
	}

	captured, err := os.ReadFile(requests)
	if err != nil {
		t.Fatal(err)
	}
	for _, expected := range []string{"\"method\":\"initialize\"", "\"method\":\"initialized\"", "\"useStateDbOnly\":true", "\"sortKey\":\"recency_at\"", "\"cursor\":\"next\"", "\"includeTurns\":false"} {
		if !strings.Contains(string(captured), expected) {
			t.Errorf("requests did not contain %s", expected)
		}
	}
}

func TestAppServerSanitizesProtocolMetadata(t *testing.T) {
	var thread protocolThread
	thread.ID = "thread\n\x1b[31m"
	thread.Name = pointer(" title\nvalue ")
	thread.Preview = "preview\nvalue"
	thread.CWD = "/tmp/project\n"
	thread.ModelProvider = "openai"
	thread.Source = []byte(`"cli"`)
	thread.Status.Type = "notLoaded"
	session, err := thread.session()
	if err != nil {
		t.Fatal(err)
	}
	if session.ID.Display() != "thread [31m" || session.Name != "title value" || session.CWD != "/tmp/project" {
		t.Fatalf("unsafe session = %#v", session)
	}
}

func TestAppServerTimeout(t *testing.T) {
	program, _ := fakeServer(t, "read_line\nsleep 1")
	_, err := start(context.Background(), program, "test", 20*time.Millisecond)
	if err == nil || !strings.Contains(err.Error(), "timed out") {
		t.Fatalf("start error = %v", err)
	}
}

func TestProtocolErrorsAreTerminalSafe(t *testing.T) {
	message := "bad\n\x1b[31m " + strings.Repeat("x", 300)
	_, err := decodeResponse([]byte(`{"jsonrpc":"2.0","id":1,"error":{"message":`+jsonString(message)+`}}`), 1)
	if err == nil {
		t.Fatal("decodeResponse() error = nil")
	}
	if got := []rune(err.Error()); len(got) > 240 || strings.ContainsAny(err.Error(), "\n\r\x1b") {
		t.Fatalf("unsafe protocol error = %q", err)
	}
}

func TestCloseCancelsActiveListRequest(t *testing.T) {
	program, requests := fakeServer(t, `
read_line
reply '{"jsonrpc":"2.0","id":1,"result":{}}'
read_line
read_line
sleep 2
`)
	source, err := start(context.Background(), program, "test", 10*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	result := make(chan error, 1)
	go func() {
		_, err := source.ListSessions(context.Background(), "")
		result <- err
	}()
	awaitRequest(t, requests, "\"method\":\"thread/list\"")
	_ = source.Close()
	select {
	case err := <-result:
		if err == nil || !strings.Contains(err.Error(), "cancelled") {
			t.Fatalf("list error = %v", err)
		}
	case <-time.After(time.Second):
		t.Fatal("active list request was not cancelled promptly")
	}
}

func TestProtocolPathIsIgnoredAndCombinedMetadataIsBounded(t *testing.T) {
	var thread protocolThread
	if err := json.Unmarshal([]byte(`{"id":"id","preview":"","cwd":"/tmp/project","createdAt":1,"updatedAt":1,"modelProvider":"`+strings.Repeat("p", 240)+`","source":"`+strings.Repeat("s", 240)+`","status":{"type":"idle"},"path":"/private/codex.jsonl"}`), &thread); err != nil {
		t.Fatal(err)
	}
	session, err := thread.session()
	if err != nil {
		t.Fatal(err)
	}
	if len([]rune(session.Provider)) > 240 || strings.Contains(session.CWD, "codex.jsonl") {
		t.Fatalf("unsafe metadata = %#v", session)
	}
}

func awaitRequest(t *testing.T, path, fragment string) {
	t.Helper()
	deadline := time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		contents, _ := os.ReadFile(path)
		if strings.Contains(string(contents), fragment) {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("request log did not contain %s", fragment)
}

func jsonString(value string) string {
	encoded, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return string(encoded)
}

func fakeServer(t *testing.T, body string) (string, string) {
	t.Helper()
	directory := t.TempDir()
	requests := filepath.Join(directory, "requests.log")
	program := filepath.Join(directory, "codex")
	script := "#!/bin/sh\n" +
		"read_line() { IFS= read -r line || exit 1; printf '%s\\n' \"$line\" >> '" + requests + "'; }\n" +
		"reply() { printf '%s\\n' \"$1\"; }\n" + body
	if err := os.WriteFile(program, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return program, requests
}

func pointer(value string) *string { return &value }
