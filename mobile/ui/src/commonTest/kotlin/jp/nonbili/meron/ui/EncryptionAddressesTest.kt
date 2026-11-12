package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountAlias
import jp.nonbili.meron.shared.AccountSummary
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.putJsonArray
import kotlin.test.Test
import kotlin.test.assertEquals

class EncryptionAddressesTest {
    @Test
    fun unfinishedAliasesCannotProducePermanentMissingKeyNotices() {
        val account =
            AccountSummary(
                id = "acc",
                email = "me@example.org",
                aliases = listOf(AccountAlias("ali"), AccountAlias("bad alias@example.org"), AccountAlias("<alias@example.org>"), AccountAlias("alias@example.org")),
            )
        assertEquals(listOf("me@example.org", "alias@example.org"), encryptionAddresses(account))
    }

    @Test
    fun setupIncludesThePrimaryAndConfiguredAliasesOnce() {
        val account =
            AccountSummary(
                id = "acc",
                email = " Ping@Nonbili.jp ",
                aliases = listOf(AccountAlias("ping@nonbili.jp"), AccountAlias("Ping@Meron.im"), AccountAlias("ping@meron.im"), AccountAlias(" ")),
            )
        assertEquals(listOf("ping@nonbili.jp", "ping@meron.im"), encryptionAddresses(account))
    }

    @Test
    fun matchingThePrimaryDoesNotHideAMissingAlias() {
        val key = buildJsonObject { putJsonArray("emails") { add("ping@nonbili.jp") } }
        assertEquals(
            listOf("ping@meron.im"),
            missingKeyAddresses(key, listOf("ping@nonbili.jp", "ping@meron.im")),
        )
    }

    @Test
    fun addingAnAliasRevealsItWithoutChangingTheKey() {
        val key = buildJsonObject { putJsonArray("emails") { add("PING@NONBILI.JP") } }
        val account = AccountSummary(id = "acc", email = "ping@nonbili.jp")
        assertEquals(emptyList(), missingKeyAddresses(key, encryptionAddresses(account)))
        val updated = account.copy(aliases = listOf(AccountAlias("ping@meron.im")))
        assertEquals(listOf("ping@meron.im"), missingKeyAddresses(key, encryptionAddresses(updated)))
    }

    @Test
    fun aKeyCoveringAllIdentitiesNeedsNoNotice() {
        val key =
            buildJsonObject {
                putJsonArray("emails") {
                    add("ping@nonbili.jp")
                    add("ping@meron.im")
                }
            }
        assertEquals(emptyList(), missingKeyAddresses(key, listOf("ping@nonbili.jp", "ping@meron.im")))
    }

    @Test
    fun nonAsciiCaseMatchesTheCoreWithoutMergingDistinctAddresses() {
        val account = AccountSummary(id = "acc", email = "Ünal@Example.org", aliases = listOf(AccountAlias("ünal@example.org")))
        val addresses = encryptionAddresses(account)
        assertEquals(listOf("Ünal@example.org", "ünal@example.org"), addresses)
        val key = buildJsonObject { putJsonArray("emails") { add("Ünal@example.org") } }
        assertEquals(listOf("ünal@example.org"), missingKeyAddresses(key, addresses))
    }
}
