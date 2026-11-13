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
                email = " Alex@Personal.example ",
                aliases = listOf(AccountAlias("alex@personal.example"), AccountAlias("Alex@Work.example"), AccountAlias("alex@work.example"), AccountAlias(" ")),
            )
        assertEquals(listOf("alex@personal.example", "alex@work.example"), encryptionAddresses(account))
    }

    @Test
    fun matchingThePrimaryDoesNotHideAMissingAlias() {
        val key = buildJsonObject { putJsonArray("emails") { add("alex@personal.example") } }
        assertEquals(
            listOf("alex@work.example"),
            missingKeyAddresses(key, listOf("alex@personal.example", "alex@work.example")),
        )
    }

    @Test
    fun addingAnAliasRevealsItWithoutChangingTheKey() {
        val key = buildJsonObject { putJsonArray("emails") { add("ALEX@PERSONAL.EXAMPLE") } }
        val account = AccountSummary(id = "acc", email = "alex@personal.example")
        assertEquals(emptyList(), missingKeyAddresses(key, encryptionAddresses(account)))
        val updated = account.copy(aliases = listOf(AccountAlias("alex@work.example")))
        assertEquals(listOf("alex@work.example"), missingKeyAddresses(key, encryptionAddresses(updated)))
    }

    @Test
    fun aKeyCoveringAllIdentitiesNeedsNoNotice() {
        val key =
            buildJsonObject {
                putJsonArray("emails") {
                    add("alex@personal.example")
                    add("alex@work.example")
                }
            }
        assertEquals(emptyList(), missingKeyAddresses(key, listOf("alex@personal.example", "alex@work.example")))
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
