package main

import (
	"os"
	"path/filepath"
)

func dragAttachment(payload map[string]any) (any, error) {
	key, _ := payload["key"].(string)
	filename, _ := payload["filename"].(string)
	src, err := mediaFilePath(key)
	if err != nil {
		return nil, err
	}
	if filename == "" {
		filename = filepath.Base(src)
	}
	root := attachmentOpenRoot(key)
	pruneAttachmentOpenDir(root)
	if err := os.MkdirAll(root, 0o700); err != nil {
		return nil, err
	}
	dir, err := os.MkdirTemp(root, "drag-")
	if err != nil {
		return nil, err
	}
	name := safeFilename(filename)
	if name == ".." {
		name = "attachment"
	}
	dest := filepath.Join(dir, name)
	if err := copyFileTo(src, dest, 0o600); err != nil {
		_ = os.RemoveAll(dir)
		return nil, err
	}
	started, err := startNativeAttachmentDrag(dest)
	if !started {
		_ = os.RemoveAll(dir)
	}
	return started, err
}
