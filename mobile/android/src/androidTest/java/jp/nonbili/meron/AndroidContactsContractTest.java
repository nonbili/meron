package jp.nonbili.meron;

import static org.junit.Assert.assertTrue;
import static org.junit.Assert.assertFalse;

import androidx.test.platform.app.InstrumentationRegistry;
import org.json.JSONObject;
import org.junit.Test;
import java.io.File;

/** Exercise the packaged phone binary, not a host-only protocol implementation. */
public final class AndroidContactsContractTest {
    @Test
    public void contactsDirectoryAndExactHistoryAreAvailableOnPhone() throws Exception {
        assertTrue("meron_core load error: " + MeronCoreNative.loadError(), MeronCoreNative.isLoaded());
        File dataDir = new File(InstrumentationRegistry.getInstrumentation()
                .getTargetContext().getCacheDir(), "contacts-contract-" + System.nanoTime());
        assertTrue(dataDir.mkdirs());
        JSONObject init = new JSONObject(MeronCoreNative.initJson(dataDir.getAbsolutePath()));
        assertTrue(init.toString(), init.getBoolean("ok"));
        try {
            JSONObject directory = invoke("contacts.suggest", "{\"account\":\"\",\"query\":\"\",\"directory\":true,\"sort\":\"name\",\"limit\":100}");
            assertTrue(directory.toString(), directory.getJSONArray("contacts").length() == 0);
            JSONObject history = invoke("mail.contactThreads", "{\"addr\":\"nobody@example.com\",\"limit\":50}");
            assertTrue(history.toString(), history.getJSONArray("threads").length() == 0);
            assertTrue(history.toString(), history.isNull("next_cursor"));
            invoke("app.prefsSet", "{\"key\":\"contact_note:nobody@example.com\",\"value\":\"**Private phone test**\"}");
            invoke("app.prefsSet", "{\"key\":\"pinned_contacts\",\"value\":[{\"name\":\"Phone test\",\"addr\":\"nobody@example.com\"}]}");
            invoke("app.prefsSet", "{\"key\":\"contacts_sort\",\"value\":\"name\"}");
            MeronCoreNative.initJson(dataDir.getAbsolutePath());
            JSONObject prefs = invoke("app.prefsGet", "{\"keys\":[\"contact_note:nobody@example.com\",\"pinned_contacts\",\"contacts_sort\"]}").getJSONObject("prefs");
            assertTrue(prefs.toString(), prefs.getString("contact_note:nobody@example.com").equals("**Private phone test**"));
            assertTrue(prefs.toString(), prefs.getJSONArray("pinned_contacts").length() == 1);
            assertTrue(prefs.toString(), prefs.getString("contacts_sort").equals("name"));
        } finally {
            // Leave the app's real account database as the active native context.
            File realDir = InstrumentationRegistry.getInstrumentation().getTargetContext().getFilesDir();
            MeronCoreNative.configureJson(realDir.getAbsolutePath(), MeronDbKey.INSTANCE.get(
                    InstrumentationRegistry.getInstrumentation().getTargetContext()));
        }
    }

    private JSONObject invoke(String method, String params) throws Exception {
        JSONObject response = new JSONObject(MeronCoreNative.invokeJson(
                "{\"id\":420,\"method\":\"" + method + "\",\"params\":" + params + "}"));
        assertFalse(response.toString(), response.has("error"));
        return response.getJSONObject("result");
    }
}
