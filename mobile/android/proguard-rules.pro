# libmeron_core.so binds to MeronCoreNative by name: its JNI exports are
# Java_jp_nonbili_meron_MeronCoreNative_*, and it calls back into the static
# dispatchCoreEventFromNative(String), which nothing on the Java side references.
-keep class jp.nonbili.meron.MeronCoreNative {
    native <methods>;
    private static void dispatchCoreEventFromNative(java.lang.String);
}

# WorkManager instantiates its Room database (WorkDatabase_Impl) reflectively
# via the no-arg constructor. Room 2.6's consumer rule keeps the class but not
# <init>, so R8 full mode strips it and the app crashes at startup.
-keep class * extends androidx.room.RoomDatabase {
    <init>();
}
