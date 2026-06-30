package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.parseChangelogResponse
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// Fetch the in-app changelog (GitHub releases atom feed, filtered to the
// mobile `android/v*` tags by the core). Re-runs on each open so a freshly
// shipped release shows without restarting the app.
internal fun MeronMobileState.loadChangelog() {
    if (!coreLoaded) {
        changelogError = true
        return
    }
    changelogLoading = true
    changelogError = false
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                requireCoreOk(MobileMailCommandClient(core).fetchChangelog())
            }
        }.onSuccess {
            changelog = parseChangelogResponse(it)
        }.onFailure {
            changelogError = true
        }
        changelogLoading = false
    }
}
