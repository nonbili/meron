package jp.nonbili.meron;

import android.app.Application;
import android.content.Context;
import androidx.test.runner.AndroidJUnitRunner;

/** Native-core tests must not register the real app's background listeners. */
public final class IsolatedCoreTestRunner extends AndroidJUnitRunner {
    @Override
    public Application newApplication(ClassLoader loader, String className, Context context)
            throws InstantiationException, IllegalAccessException, ClassNotFoundException {
        return super.newApplication(loader, Application.class.getName(), context);
    }

    @Override
    public void onStart() {
        MeronCoreNative.initializeAndroidContext(getTargetContext());
        super.onStart();
    }
}
