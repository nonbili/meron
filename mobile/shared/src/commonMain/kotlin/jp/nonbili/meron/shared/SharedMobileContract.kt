package jp.nonbili.meron.shared

object SharedMobileContract {
    const val protocolVersion: Int = EXPECTED_PROTOCOL_VERSION

    fun pingJson(id: Long = 1): String = pingRequest(id).toJson()

    fun transportName(coreLoaded: Boolean): String {
        return if (coreLoaded) "Rust core" else "Java fallback"
    }
}
