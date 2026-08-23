package domain

import (
	"cmp"
	"errors"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"
	"unicode"
)

const maxTerminalTextRunes = 240

// SessionID is an opaque Codex thread identifier. Its display form is always
// terminal-safe, while the original value remains available for JSON-RPC.
type SessionID string

func NewSessionID(value string) (SessionID, error) {
	if strings.TrimSpace(value) == "" {
		return "", errors.New("session id is empty")
	}
	return SessionID(value), nil
}

func (id SessionID) String() string  { return string(id) }
func (id SessionID) Display() string { return SanitizeTerminalText(string(id)) }

type Session struct {
	ID        SessionID
	Name      string
	Preview   string
	CWD       string
	CreatedAt int64
	UpdatedAt int64
	RecencyAt int64
	Provider  string
	Status    string
	Branch    string
}

func (session Session) Title() string {
	if session.Name == "" {
		return "Untitled session"
	}
	return session.Name
}

func (session Session) ProjectLabel() string { return ProjectLabel(session.CWD) }

type SessionPage struct {
	Sessions   []Session
	NextCursor string
}

func SortSessions(sessions []Session) {
	slices.SortFunc(sessions, func(left, right Session) int {
		if result := cmp.Compare(right.RecencyAt, left.RecencyAt); result != 0 {
			return result
		}
		if result := cmp.Compare(right.UpdatedAt, left.UpdatedAt); result != 0 {
			return result
		}
		return cmp.Compare(left.ID.String(), right.ID.String())
	})
}

func SanitizeTerminalText(value string) string {
	var result strings.Builder
	result.Grow(min(len(value), maxTerminalTextRunes))
	pendingSpace := false
	characters := 0
	for _, character := range value {
		if unicode.IsControl(character) || unicode.IsSpace(character) {
			pendingSpace = result.Len() > 0
			continue
		}
		if pendingSpace {
			if characters+1 >= maxTerminalTextRunes {
				break
			}
			result.WriteByte(' ')
			characters++
			pendingSpace = false
		}
		if characters == maxTerminalTextRunes {
			break
		}
		result.WriteRune(character)
		characters++
	}
	return result.String()
}

func ProjectLabel(cwd string) string {
	clean := filepath.Clean(cwd)
	label := filepath.Base(clean)
	if label == "." && cwd == "" {
		return ""
	}
	parent := filepath.Base(filepath.Dir(clean))
	if parent == "Users" || parent == "home" {
		return "Workspace"
	}
	return label
}

func DisplayProvider(provider string) string {
	if strings.EqualFold(strings.Split(provider, "/")[0], "openai") {
		return "OpenAI"
	}
	return provider
}

func DisplayStatus(status string) string {
	if status == "" {
		return "Unknown"
	}
	var result strings.Builder
	previousLowercase := false
	for _, character := range status {
		if character == '-' || character == '_' {
			if result.Len() > 0 && !strings.HasSuffix(result.String(), " ") {
				result.WriteByte(' ')
			}
			previousLowercase = false
			continue
		}
		if unicode.IsUpper(character) && previousLowercase {
			result.WriteByte(' ')
		}
		result.WriteRune(unicode.ToLower(character))
		previousLowercase = unicode.IsLower(character)
	}
	text := result.String()
	if text == "" {
		return "Unknown"
	}
	runes := []rune(text)
	runes[0] = unicode.ToUpper(runes[0])
	return string(runes)
}

func FormatAge(timestamp int64, now time.Time) string {
	seconds := max(now.Unix()-timestamp, 0)
	switch {
	case seconds < 60:
		return "now"
	case seconds < 60*60:
		return itoa(seconds/60) + "m"
	case seconds < 24*60*60:
		return itoa(seconds/(60*60)) + "h"
	case seconds < 30*24*60*60:
		return itoa(seconds/(24*60*60)) + "d"
	default:
		return itoa(seconds/(30*24*60*60)) + "mo"
	}
}

func itoa(value int64) string {
	return strconv.FormatInt(value, 10)
}
