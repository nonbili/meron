//go:build !darwin && (!linux || !gtk3 || bindings)

package main

// Other platforms print directly in the frontend so JS errors reach its caller.
func printNativeMail(html string) (bool, string, error) {
	return false, "", nil
}
