# libmeron_core.so binds to MeronCoreNative by name: its JNI exports are
# Java_jp_nonbili_meron_MeronCoreNative_*, and it calls back into the static
# dispatchCoreEventFromNative(String), which nothing on the Java side references.
-keep class jp.nonbili.meron.MeronCoreNative {
    native <methods>;
    private static void dispatchCoreEventFromNative(java.lang.String);
}
