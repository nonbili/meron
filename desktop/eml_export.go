package main

import (
	"archive/zip"
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"mime"
	"net/mail"
	"os"
	"path/filepath"
	"strings"
	"time"

	wailsRuntime "meron/internal/desktopruntime"
)

// Messages fetched per sidecar call. Small enough that progress and cancel stay
// responsive and a batch of attachment-heavy mail still fits its call timeout.
const emlExportBatchSize = 10

// After this many batches in a row fail outright the connection is taken to be
// gone, and the rest are reported as skipped instead of each waiting to time out.
const emlExportMaxConsecutiveErrors = 3

// emlExportSource is the messages to export from one mailbox.
type emlExportSource struct {
	Account string
	Folder  string
	UIDs    []uint32
}

// emlBatchFetcher writes each requested message to "<dir>/<uid>.eml" and
// returns the UIDs it wrote; a message the server no longer has is left out.
type emlBatchFetcher func(source emlExportSource, uids []uint32, dir string) ([]uint32, error)

type emlExportResult struct {
	Exported  int
	Failed    int
	Cancelled bool
}

func (a *App) beginEmlExport() (context.Context, error) {
	a.emlExportMu.Lock()
	defer a.emlExportMu.Unlock()
	if a.emlExportCancel != nil {
		return nil, errors.New("an export is already running")
	}
	ctx, cancel := context.WithCancel(context.Background())
	a.emlExportCancel = cancel
	return ctx, nil
}

func (a *App) endEmlExport() {
	a.emlExportMu.Lock()
	defer a.emlExportMu.Unlock()
	if a.emlExportCancel != nil {
		a.emlExportCancel()
		a.emlExportCancel = nil
	}
}

func (a *App) cancelEmlExport() {
	a.emlExportMu.Lock()
	defer a.emlExportMu.Unlock()
	if a.emlExportCancel != nil {
		a.emlExportCancel()
	}
}

// exportEml saves the messages of the given threads, or of a whole folder, as
// .eml files: a lone message goes straight to a .eml, anything more into one
// .zip. Raw bytes aren't cached locally, so every message is refetched over
// IMAP in small batches; progress is pushed as "mail.exportProgress" events and
// "mail.exportEmlCancel" stops the run between batches. A message that can't be
// fetched is skipped and counted rather than failing the export.
func (a *App) exportEml(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, a.engineUnavailable()
	}
	ctx, err := a.beginEmlExport()
	if err != nil {
		return nil, err
	}
	defer a.endEmlExport()

	sources, unresolved, err := a.emlExportSources(payload)
	if err != nil {
		return nil, err
	}
	total := 0
	for _, source := range sources {
		total += len(source.UIDs)
	}
	if total == 0 {
		return nil, errors.New("no messages to export")
	}

	name, _ := payload["name"].(string)
	options := wailsRuntime.SaveDialogOptions{
		Title:                "Save message",
		DefaultFilename:      emlFilename(name),
		CanCreateDirectories: true,
	}
	if total > 1 {
		options.Title = "Export messages"
		options.DefaultFilename = safeFilenameBase(name, "messages") + ".zip"
		options.Filters = []wailsRuntime.FileFilter{{DisplayName: "Zip archive (*.zip)", Pattern: "*.zip"}}
	}
	dest, err := saveFileDialog(a.ctx, options)
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}

	emit := func(done, failed int) {
		wailsRuntime.EventsEmit(a.ctx, "mail.exportProgress", map[string]any{
			"done": done, "failed": failed, "total": total,
		})
	}
	emit(0, 0)
	result, err := runEmlExport(ctx, sources, dest, a.fetchEmlBatch, emit)
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"saved":     !result.Cancelled,
		"cancelled": result.Cancelled,
		"path":      dest,
		"exported":  result.Exported,
		"failed":    result.Failed + unresolved,
	}, nil
}

// emlExportSources resolves an export request into per-mailbox UID lists and
// the number of requested threads that have nothing exportable (feed items,
// rows the cache no longer knows). A folder export asks the server for every
// UID, so it isn't limited to what has been synced.
func (a *App) emlExportSources(payload map[string]any) ([]emlExportSource, int, error) {
	if folderID, _ := payload["folder_id"].(string); folderID != "" {
		accountID, _ := payload["account_id"].(string)
		if accountID == "" || accountID == "unified" || isRSSAccountID(accountID) {
			return nil, 0, errors.New("this folder cannot be exported")
		}
		res, err := a.sidecar.Call("messages.exportUids", map[string]any{
			"account": accountID, "folder": folderID, "all": true,
		})
		if err != nil {
			return nil, 0, err
		}
		return []emlExportSource{{Account: accountID, Folder: folderID, UIDs: uidsFromResult(res, "uids")}}, 0, nil
	}

	threads, _ := payload["threads"].([]any)
	var sources []emlExportSource
	index := map[string]int{}
	seen := map[string]bool{}
	unresolved := 0
	for _, item := range threads {
		thread, _ := item.(map[string]any)
		threadID, _ := thread["thread_id"].(string)
		ids, ok := parseImapThreadID(threadID)
		if !ok {
			unresolved++
			continue
		}
		folder := deleteFolder(thread, ids.Folder)
		// Branch-compound thread keys pass through untouched; the sidecar
		// splits them and scopes the export to the subject branch.
		params := map[string]any{"account": ids.Account, "folder": folder, "thread_key": ids.ThreadKey}
		if ids.ThreadKey == "" {
			params = map[string]any{"account": ids.Account, "folder": folder, "uid": ids.UID}
		}
		res, err := a.sidecar.Call("messages.exportUids", params)
		uids := uidsFromResult(res, "uids")
		if err != nil || len(uids) == 0 {
			unresolved++
			continue
		}
		key := ids.Account + "\x00" + folder
		at, ok := index[key]
		if !ok {
			at = len(sources)
			index[key] = at
			sources = append(sources, emlExportSource{Account: ids.Account, Folder: folder})
		}
		for _, uid := range uids {
			if uidKey := fmt.Sprintf("%s\x00%d", key, uid); !seen[uidKey] {
				seen[uidKey] = true
				sources[at].UIDs = append(sources[at].UIDs, uid)
			}
		}
	}
	return sources, unresolved, nil
}

func uidsFromResult(res any, key string) []uint32 {
	object, _ := res.(map[string]any)
	list, _ := object[key].([]any)
	uids := make([]uint32, 0, len(list))
	for _, item := range list {
		if n, ok := item.(float64); ok && n > 0 {
			uids = append(uids, uint32(n))
		}
	}
	return uids
}

func (a *App) fetchEmlBatch(source emlExportSource, uids []uint32, dir string) ([]uint32, error) {
	res, err := a.sidecar.Call("messages.saveRawBatch", map[string]any{
		"account": source.Account, "folder": source.Folder, "uids": uids, "dir": dir,
	})
	if err != nil {
		return nil, err
	}
	return uidsFromResult(res, "saved"), nil
}

// runEmlExport fetches every source message through fetch and writes it to
// dest: the bare message when there is exactly one, a zip of them otherwise.
// progress is called after each batch with the running exported and skipped
// counts. The export is built in a staging file and only replaces dest once it
// has succeeded, so a cancelled or empty export leaves dest as it was.
func runEmlExport(ctx context.Context, sources []emlExportSource, dest string, fetch emlBatchFetcher, progress func(done, failed int)) (result emlExportResult, err error) {
	total := 0
	for _, source := range sources {
		total += len(source.UIDs)
	}
	tmp, err := os.MkdirTemp("", "meron-eml-")
	if err != nil {
		return result, fmt.Errorf("prepare export: %w", err)
	}
	defer os.RemoveAll(tmp)

	// Staged next to dest so finishing is an atomic rename. Under a sandboxed
	// file-chooser portal only the chosen path itself is writable; the staging
	// file then lives in the temp dir and is copied over dest at the end.
	out, err := os.CreateTemp(filepath.Dir(dest), ".meron-export-*")
	if err != nil {
		if out, err = os.CreateTemp(tmp, "export-*"); err != nil {
			return result, fmt.Errorf("prepare export: %w", err)
		}
	}
	staged := out.Name()
	defer os.Remove(staged)
	defer out.Close()
	var archive *zip.Writer
	if total > 1 {
		archive = zip.NewWriter(out)
	}

	names := map[string]bool{}
	// Counted per account: one unreachable server must not stop the others.
	consecutiveErrors := map[string]int{}
	var fetchErr error
	for _, source := range sources {
		for start := 0; start < len(source.UIDs); start += emlExportBatchSize {
			if ctx.Err() != nil {
				result.Cancelled = true
				return result, nil
			}
			batch := source.UIDs[start:min(start+emlExportBatchSize, len(source.UIDs))]
			if consecutiveErrors[source.Account] >= emlExportMaxConsecutiveErrors {
				result.Failed += len(batch)
				continue
			}
			saved, batchErr := fetch(source, batch, tmp)
			// Cancel can land while the fetch is in flight, the last one included.
			if ctx.Err() != nil {
				result.Cancelled = true
				return result, nil
			}
			if batchErr != nil {
				consecutiveErrors[source.Account]++
				fetchErr = batchErr
				result.Failed += len(batch)
				progress(result.Exported, result.Failed)
				continue
			}
			consecutiveErrors[source.Account] = 0
			wanted := make(map[uint32]bool, len(batch))
			for _, uid := range batch {
				wanted[uid] = true
			}
			written := 0
			for _, uid := range saved {
				// Packing a batch of large messages takes a while too.
				if ctx.Err() != nil {
					result.Cancelled = true
					return result, nil
				}
				if !wanted[uid] {
					continue
				}
				delete(wanted, uid)
				path := filepath.Join(tmp, fmt.Sprintf("%d.eml", uid))
				if archive != nil {
					err = addEmlToZip(archive, path, names)
				} else {
					err = copyFileInto(out, path)
				}
				_ = os.Remove(path)
				if err != nil {
					return result, fmt.Errorf("write export: %w", err)
				}
				written++
			}
			result.Exported += written
			result.Failed += len(batch) - written
			progress(result.Exported, result.Failed)
		}
	}
	// Last chance to back out: past this point dest is replaced.
	if ctx.Err() != nil {
		result.Cancelled = true
		return result, nil
	}
	if result.Exported == 0 {
		if fetchErr != nil {
			return result, fetchErr
		}
		return result, errors.New("no messages could be exported")
	}
	if archive != nil {
		if err := archive.Close(); err != nil {
			return result, fmt.Errorf("write export: %w", err)
		}
	}
	if err := out.Close(); err != nil {
		return result, fmt.Errorf("write export: %w", err)
	}
	if err := publishEmlExport(staged, dest); err != nil {
		return result, fmt.Errorf("write export: %w", err)
	}
	return result, nil
}

// publishEmlExport moves the finished staging file over dest, or copies it when
// the two aren't on the same filesystem (the sandboxed-portal case).
func publishEmlExport(staged, dest string) error {
	if filepath.Dir(staged) == filepath.Dir(dest) {
		// CreateTemp's 0600 is right for mail; keep it rather than widening.
		return os.Rename(staged, dest)
	}
	out, err := os.OpenFile(dest, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		return err
	}
	if err := copyFileInto(out, staged); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

func copyFileInto(out io.Writer, path string) error {
	in, err := os.Open(path)
	if err != nil {
		return err
	}
	defer in.Close()
	_, err = io.Copy(out, in)
	return err
}

// addEmlToZip stores the message at path under a name built from its own Date
// and Subject headers, stamped with that date.
func addEmlToZip(archive *zip.Writer, path string, names map[string]bool) error {
	in, err := os.Open(path)
	if err != nil {
		return err
	}
	defer in.Close()

	base, date := emlArchiveName(bufio.NewReader(in))
	if _, err := in.Seek(0, io.SeekStart); err != nil {
		return err
	}
	if date.IsZero() {
		date = time.Now()
	}
	entry, err := archive.CreateHeader(&zip.FileHeader{
		Name:     uniqueArchiveName(base, ".eml", names),
		Method:   zip.Deflate,
		Modified: date,
	})
	if err != nil {
		return err
	}
	_, err = io.Copy(entry, in)
	return err
}

// emlArchiveName derives "2026-10-04_Subject" from a message's headers, with
// the date left off when the message has none that parses.
func emlArchiveName(r io.Reader) (string, time.Time) {
	var subject string
	var date time.Time
	if message, err := mail.ReadMessage(r); err == nil {
		subject = message.Header.Get("Subject")
		if decoded, err := new(mime.WordDecoder).DecodeHeader(subject); err == nil {
			subject = decoded
		}
		date, _ = message.Header.Date()
	}
	base := safeFilenameBase(subject, "message")
	if !date.IsZero() {
		base = date.Format("2006-01-02") + "_" + base
	}
	return base, date
}

// uniqueArchiveName suffixes a repeated name ("_2", "_3", ...). Names are
// compared case-insensitively so the archive also unpacks cleanly onto
// case-insensitive filesystems.
func uniqueArchiveName(base, ext string, names map[string]bool) string {
	name := base + ext
	for n := 2; names[strings.ToLower(name)]; n++ {
		name = fmt.Sprintf("%s_%d%s", base, n, ext)
	}
	names[strings.ToLower(name)] = true
	return name
}
