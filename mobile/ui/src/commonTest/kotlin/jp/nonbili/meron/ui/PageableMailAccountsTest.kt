package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class PageableMailAccountsTest {
    @Test
    fun unifiedOpaqueCursorMakesIncludedAccountsPageable() {
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
                mailboxCursor = "unified:opaque",
            )

        assertEquals(listOf("acc1", "acc2"), pageable.map { it.id })
    }

    @Test
    fun unifiedWithOnlyExcludedAccountsIsNotPageable() {
        val pageable =
            pageableMailAccounts(
                selectedAccountId = UNIFIED_ACCOUNT_ID,
                accounts = listOf(account("acc1", includedInUnified = false)),
                mailboxCursor = "unified:opaque",
            )

        assertTrue(pageable.isEmpty())
    }

    @Test
    fun unifiedRequiresOpaqueCursor() {
        val pageable =
            pageableMailAccounts(
                selectedAccountId = UNIFIED_ACCOUNT_ID,
                accounts = listOf(account("acc1")),
                mailboxCursor = "",
            )

        assertTrue(pageable.isEmpty())
    }

    @Test
    fun blankSelectionFallsBackToUnified() {
        val pageable =
            pageableMailAccounts(
                selectedAccountId = "",
                accounts = listOf(account("acc1")),
                mailboxCursor = "unified:opaque",
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
            ).map { it.id },
        )
        assertTrue(
            pageableMailAccounts(
                selectedAccountId = "acc1",
                accounts = accounts,
                mailboxCursor = "",
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
            ).isEmpty(),
        )
    }

    private fun account(
        id: String,
        includedInUnified: Boolean = true,
    ): AccountSummary = AccountSummary(id = id, email = "$id@example.com", includedInUnified = includedInUnified)
}
