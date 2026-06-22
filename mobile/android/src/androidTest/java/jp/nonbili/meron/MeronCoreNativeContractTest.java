package jp.nonbili.meron;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import androidx.test.platform.app.InstrumentationRegistry;

import org.json.JSONObject;
import org.junit.Test;

import java.io.File;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

public final class MeronCoreNativeContractTest {
    @Test
    public void packagedRustCoreMatchesProtocolAndHandlesPing() throws Exception {
        assertTrue("meron_core load error: " + MeronCoreNative.loadError(), MeronCoreNative.isLoaded());
        assertEquals(1, MeronCoreNative.protocolVersion());

        File dataDir = InstrumentationRegistry.getInstrumentation()
                .getTargetContext()
                .getFilesDir();
        JSONObject init = new JSONObject(MeronCoreNative.initJson(dataDir.getAbsolutePath()));
        assertTrue(init.getBoolean("ok"));
        assertEquals(1, init.getInt("protocol"));
        assertEquals(dataDir.getAbsolutePath(), init.getString("data_dir"));

        JSONObject ping = new JSONObject(MeronCoreNative.invokeJson("{\"id\":41,\"method\":\"ping\",\"params\":{}}"));
        assertEquals(41, ping.getInt("id"));
        assertTrue(ping.getJSONObject("result").getBoolean("pong"));
        assertEquals(1, ping.getJSONObject("result").getInt("protocol"));
    }

    @Test
    public void packagedRustCoreDeliversReadyEventThroughJni() throws Exception {
        assertTrue("meron_core load error: " + MeronCoreNative.loadError(), MeronCoreNative.isLoaded());

        CountDownLatch ready = new CountDownLatch(1);
        AtomicReference<String> eventJson = new AtomicReference<>("");
        MeronCoreNative.CoreEventListener listener = event -> {
            eventJson.set(event);
            ready.countDown();
        };

        MeronCoreNative.addCoreEventListener(listener);
        try {
            assertTrue(MeronCoreNative.emitReadyEvent());
            assertTrue("ready event was not delivered", ready.await(5, TimeUnit.SECONDS));
        } finally {
            MeronCoreNative.removeCoreEventListener(listener);
        }

        assertFalse(eventJson.get().isEmpty());
        JSONObject envelope = new JSONObject(eventJson.get());
        assertEquals("ready", envelope.getString("event"));
        assertEquals(1, envelope.getJSONObject("detail").getInt("protocol"));
    }
}
