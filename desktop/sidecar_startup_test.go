package main

import (
	"strings"
	"testing"
	"time"
)

func startingSidecar() *Sidecar {
	coreUp := make(chan struct{})
	release := func() { close(coreUp) }
	return &Sidecar{started: true, pending: map[uint64]chan sidecarResponse{}, coreUp: coreUp, releaseCore: release}
}

func TestAwaitCoreOutlastsTimeoutWhileMigrating(t *testing.T) {
	s := startingSidecar()
	s.migrating = true
	done := make(chan error, 1)
	go func() { done <- s.awaitCore("account.list", 10*time.Millisecond) }()
	select {
	case err := <-done:
		t.Fatalf("awaitCore returned during a migration: %v", err)
	case <-time.After(60 * time.Millisecond):
	}
	s.finishStarting(nil)
	if err := <-done; err != nil {
		t.Fatalf("awaitCore after migration: %v", err)
	}
	if s.Migrating() {
		t.Fatal("still migrating after finishStarting")
	}
}

func TestAwaitCoreOutlastsTimeoutWhileAwaitingKeychain(t *testing.T) {
	s := startingSidecar()
	s.awaitingKeychain = true
	done := make(chan error, 1)
	go func() { done <- s.awaitCore("account.list", 10*time.Millisecond) }()
	select {
	case err := <-done:
		t.Fatalf("awaitCore returned while the keychain was being waited on: %v", err)
	case <-time.After(60 * time.Millisecond):
	}
	s.finishStarting(nil)
	if err := <-done; err != nil {
		t.Fatalf("awaitCore after the keychain answered: %v", err)
	}
	if s.awaitingKeychainUnlock() {
		t.Fatal("still awaiting the keychain after finishStarting")
	}
}

func TestAwaitCoreTimesOutOnOtherStalls(t *testing.T) {
	s := startingSidecar()
	err := s.awaitCore("account.list", 10*time.Millisecond)
	if err == nil || !strings.HasPrefix(err.Error(), "sidecar account.list timeout after ") {
		t.Fatalf("got %v; the frontend's retry matches on this message", err)
	}
}

func TestAwaitCoreSkipsSidecarNeverStarted(t *testing.T) {
	s := &Sidecar{}
	if err := s.awaitCore("account.list", time.Hour); err != nil {
		t.Fatal(err)
	}
}

func TestAwaitCoreHoldsCallsWhileANewerStoreAwaitsTheUser(t *testing.T) {
	s := startingSidecar()
	s.newerSchema = map[string]any{"found": 40.0, "supported": 34.0}
	done := make(chan error, 1)
	go func() { done <- s.awaitCore("account.list", 10*time.Millisecond) }()
	select {
	case err := <-done:
		t.Fatalf("awaitCore returned while the user was choosing: %v", err)
	case <-time.After(60 * time.Millisecond):
	}
	// The choice itself goes straight through.
	if err := s.awaitCore("store.openNewer", 10*time.Millisecond); err != nil {
		t.Fatalf("store.openNewer was held: %v", err)
	}
	s.finishStarting(nil)
	if err := <-done; err != nil {
		t.Fatalf("awaitCore after the store opened: %v", err)
	}
	if s.NewerSchema() != nil {
		t.Fatal("still waiting on the user after finishStarting")
	}
}
