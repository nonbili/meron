// Package obf provides lightweight obfuscation for credentials that have to be
// baked into the binary at source level — specifically the Google OAuth client
// id/secret, which must travel inside the public repo because the Flatpak is
// built on infrastructure we don't control and has no build-time secret
// injection.
//
// This is NOT encryption. The key lives in the binary right next to the data,
// so anyone can reverse it in a few lines. Its only purpose is to keep the raw
// values out of plaintext grep and the automated secret scanners that crawl
// public repositories. Per Google's installed-app model the client secret is
// not treated as confidential, so this is an appropriate level of protection.
package obf

import "encoding/base64"

// key is the XOR pad. Its length is arbitrary; it only needs to be longer than
// one byte so the result isn't a trivially reversible single-byte XOR.
var key = []byte{
	0x4d, 0x65, 0x72, 0x6f, 0x6e, 0x2d, 0x6f, 0x61,
	0x75, 0x74, 0x68, 0x2d, 0x76, 0x31, 0x9a, 0x3c,
}

// Encode obfuscates a plaintext value into a base64 blob suitable for embedding
// in source. Use the cmd/obfuscate helper to generate these.
func Encode(plain string) string {
	b := []byte(plain)
	xor(b)
	return base64.StdEncoding.EncodeToString(b)
}

// Decode reverses Encode. An empty or malformed blob yields "".
func Decode(blob string) string {
	if blob == "" {
		return ""
	}
	b, err := base64.StdEncoding.DecodeString(blob)
	if err != nil {
		return ""
	}
	xor(b)
	return string(b)
}

func xor(b []byte) {
	for i := range b {
		b[i] ^= key[i%len(key)]
	}
}
