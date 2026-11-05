//go:build production

package desktopruntime

import "context"

func Environment(context.Context) struct{ BuildType string } {
	return struct{ BuildType string }{"production"}
}
