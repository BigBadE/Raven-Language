package bigbade.raven.ravenintellijplugin.natives;

public class NativeParserRunner {
    public static native long start(String buffer, int startOffset, int endOffset, int initialState);

    public static native int getState(long id);

    public static native int getTokenType(long id);

    public static native int getTokenStart(long id);

    public static native int getTokenEnd(long id);

    public static native void advance(long id);

    public static native long getCurrentPosition(long id);

    public static native void restore(long position, long id);

    public static native int getPositionOffset(long position);

    public static native int getPositionState(long position);
}
