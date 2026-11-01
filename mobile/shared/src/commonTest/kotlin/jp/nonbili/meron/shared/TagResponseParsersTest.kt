package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class TagResponseParsersTest {
    @Test
    fun listsTagsMostUsedFirstWithTheirCounts() {
        val answer = """{"tags":[{"tag":"rare","count":1},{"tag":"work","count":5}]}"""
        assertEquals(listOf(TagCount("work", 5), TagCount("rare", 1)), parseTagCountsResponse(answer))
        assertEquals(listOf("work", "rare"), parseTagListResponse(answer))
    }

    @Test
    fun readsTheNameAReplacementKept() {
        val renamed = """{"ok":true,"tag":"new","rules":[],"queries":["tag:new","from:a"]}"""
        assertEquals("new", parseReplaceTagResponse(renamed))
        assertEquals(listOf("tag:new", "from:a"), parseReplacedQueries(renamed))
        assertNull(parseReplaceTagResponse("""{"ok":true,"tag":null,"rules":[{"id":"r","query":"from:a","tags":["keep"]}]}"""))
    }
}
