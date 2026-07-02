package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class PageableMailAccountsTest {
    @Test
    fun unifiedRequiresIncludedAccountWithCursor() {
        val accounts =
            listOf(
                account("acc1"),
                account("acc2"),
                account("acc3", includedInUnified = false),
            )

        val pageable =
            pageableMailAccounts(
                selectedAccountId = UNIFIED_ACCOUNT_ID,
                accounts = accounts,
                mailboxCursor = "",
                accountCursors = mapOf("acc2" to "cursor-2", "acc3" to "cursor-3"),
            )

        assertEquals(listOf("acc2"), pageable.map { it.id })
    }

    @Test
    fun unifiedWithOnlyExcludedAccountCursorsIsNotPageable() {
        // Regression: the load-more button used to show whenever any account
        // cursor existed, while the loader filtered to unified accounts and
        // silently no-opped.
        val pageable =
            pageableMailAccounts(
                selectedAccountId = UNIFIED_ACCOUNT_ID,
                accounts = listOf(account("acc1", includedInUnified = false)),
                mailboxCursor = "",
                accountCursors = mapOf("acc1" to "cursor-1"),
            )

        assertTrue(pageable.isEmpty())
    }

    @Test
    fun unifiedIgnoresBlankCursors() {
        val pageable =
            pageableMailAccounts(
                selectedAccountId = UNIFIED_ACCOUNT_ID,
                accounts = listOf(account("acc1")),
                mailboxCursor = "",
                accountCursors = mapOf("acc1" to ""),
            )

        assertTrue(pageable.isEmpty())
    }

    @Test
    fun blankSelectionFallsBackToUnified() {
        val pageable =
            pageableMailAccounts(
                selectedAccountId = "",
                accounts = listOf(account("acc1")),
                mailboxCursor = "",
                accountCursors = mapOf("acc1" to "cursor-1"),
            )

        assertEquals(listOf("acc1"), pageable.map { it.id })
    }

    @Test
    fun singleAccountUsesMailboxCursor() {
        val accounts = listOf(account("acc1"), account("acc2"))

        assertEquals(
            listOf("acc1"),
            pageableMailAccounts(
                selectedAccountId = "acc1",
                accounts = accounts,
                mailboxCursor = "cursor-1",
                accountCursors = emptyMap(),
            ).map { it.id },
        )
        assertTrue(
            pageableMailAccounts(
                selectedAccountId = "acc1",
                accounts = accounts,
                mailboxCursor = "",
                accountCursors = mapOf("acc1" to "stale"),
            ).isEmpty(),
        )
    }

    @Test
    fun singleAccountMissingFromAccountsIsNotPageable() {
        assertTrue(
            pageableMailAccounts(
                selectedAccountId = "gone",
                accounts = listOf(account("acc1")),
                mailboxCursor = "cursor-1",
                accountCursors = emptyMap(),
            ).isEmpty(),
        )
    }

    private fun account(
        id: String,
        includedInUnified: Boolean = true,
    ): AccountSummary = AccountSummary(id = id, email = "$id@example.com", includedInUnified = includedInUnified)
}
