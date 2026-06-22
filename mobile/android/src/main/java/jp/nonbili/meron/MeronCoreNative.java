package jp.nonbili.meron;

import java.util.concurrent.CopyOnWriteArrayList;

final class MeronCoreNative {
    interface CoreEventListener {
        void onCoreEventJson(String eventJson);
    }

    private static final boolean LOADED;
    private static final String LOAD_ERROR;
    private static final CopyOnWriteArrayList<CoreEventListener> EVENT_LISTENERS = new CopyOnWriteArrayList<>();
    private static boolean eventCallbackRegistered = false;

    static {
        boolean loaded = false;
        String error = "";
        try {
            System.loadLibrary("meron_core");
            loaded = true;
        } catch (UnsatisfiedLinkError e) {
            error = e.getMessage();
        }
        LOADED = loaded;
        LOAD_ERROR = error == null ? "" : error;
    }

    private MeronCoreNative() {}

    static boolean isLoaded() {
        return LOADED;
    }

    static String loadError() {
        return LOAD_ERROR;
    }

    static int protocolVersion() {
        if (!LOADED) return -1;
        return meronCoreProtocolVersion();
    }

    static String pingJson() {
        if (!LOADED) return "";
        return invokeJson("{\"id\":1,\"method\":\"ping\"}");
    }

    static String invokeJson(String requestJson) {
        if (!LOADED) return "";
        return meronCoreInvokeJson(requestJson);
    }

    static String initJson(String dataDir) {
        if (!LOADED) return "";
        return meronCoreInitJson(dataDir);
    }

    static String initJson(String dataDir, String dbKey) {
        if (!LOADED) return "";
        return meronCoreInitJsonKeyed(dataDir, dbKey);
    }

    static void addCoreEventListener(CoreEventListener listener) {
        if (!LOADED) return;
        ensureEventCallbackRegistered();
        EVENT_LISTENERS.addIfAbsent(listener);
    }

    static void removeCoreEventListener(CoreEventListener listener) {
        EVENT_LISTENERS.remove(listener);
        if (LOADED && EVENT_LISTENERS.isEmpty() && eventCallbackRegistered) {
            meronCoreUnregisterEventCallback();
            eventCallbackRegistered = false;
        }
    }

    static boolean emitReadyEvent() {
        if (!LOADED) return false;
        ensureEventCallbackRegistered();
        return meronCoreEmitReadyEvent();
    }

    private static void ensureEventCallbackRegistered() {
        if (!eventCallbackRegistered) {
            meronCoreRegisterEventCallback();
            eventCallbackRegistered = true;
        }
    }

    @SuppressWarnings("unused")
    private static void dispatchCoreEventFromNative(String eventJson) {
        for (CoreEventListener listener : EVENT_LISTENERS) {
            listener.onCoreEventJson(eventJson);
        }
    }

    private static native int meronCoreProtocolVersion();
    private static native String meronCoreInitJson(String dataDir);
    private static native String meronCoreInitJsonKeyed(String dataDir, String dbKey);
    private static native String meronCoreInvokeJson(String requestJson);
    private static native void meronCoreRegisterEventCallback();
    private static native void meronCoreUnregisterEventCallback();
    private static native boolean meronCoreEmitReadyEvent();
}
