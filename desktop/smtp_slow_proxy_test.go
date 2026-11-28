//go:build integration

package main

import (
	"bytes"
	"net"
	"strconv"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// slowSMTPProxy sits in front of a real SMTP server and holds back the
// server's answer to the end of each message's DATA, the way GMX and Web.de
// do while they scan it. The server has the message by then; only the client
// is kept waiting.
type slowSMTPProxy struct {
	port      int
	delay     time.Duration
	submitted atomic.Int32
}

func startSlowSMTPProxy(t *testing.T, upstreamPort int, delay time.Duration) *slowSMTPProxy {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	proxy := &slowSMTPProxy{port: listener.Addr().(*net.TCPAddr).Port, delay: delay}
	// Counts the accept loop too, so no Add can come after Wait has seen zero.
	var conns sync.WaitGroup
	t.Cleanup(func() {
		listener.Close()
		conns.Wait()
	})
	conns.Add(1)
	go func() {
		defer conns.Done()
		for {
			client, err := listener.Accept()
			if err != nil {
				return
			}
			conns.Add(1)
			go func() {
				defer conns.Done()
				proxy.serve(client, upstreamPort)
			}()
		}
	}()
	return proxy
}

func (p *slowSMTPProxy) serve(client net.Conn, upstreamPort int) {
	defer client.Close()
	server, err := net.Dial("tcp", net.JoinHostPort("127.0.0.1", strconv.Itoa(upstreamPort)))
	if err != nil {
		return
	}
	defer server.Close()
	// Messages whose end the server has been sent but whose answer is still
	// to be held back.
	var ended atomic.Int32
	go func() {
		defer server.Close()
		var tail []byte
		buf := make([]byte, 32*1024)
		for {
			n, err := client.Read(buf)
			if n > 0 {
				// The end-of-data marker may straddle two reads.
				window := append(tail, buf[:n]...)
				ends := int32(bytes.Count(window, []byte("\r\n.\r\n")))
				if ends > 0 {
					p.submitted.Add(ends)
					ended.Add(ends)
				}
				// Keep what could start a marker, but not one already counted.
				tail = append([]byte{}, window[max(0, len(window)-4):]...)
				if bytes.HasSuffix(window, []byte("\r\n.\r\n")) {
					tail = nil
				}
				if _, err := server.Write(buf[:n]); err != nil {
					return
				}
			}
			if err != nil {
				return
			}
		}
	}()
	buf := make([]byte, 32*1024)
	for {
		n, err := server.Read(buf)
		if n > 0 {
			if ended.Load() > 0 {
				ended.Add(-1)
				time.Sleep(p.delay)
			}
			if _, err := client.Write(buf[:n]); err != nil {
				return
			}
		}
		if err != nil {
			return
		}
	}
}
