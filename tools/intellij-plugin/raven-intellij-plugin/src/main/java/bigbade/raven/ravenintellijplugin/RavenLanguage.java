package bigbade.raven.ravenintellijplugin;

import com.intellij.lang.Language;

public class RavenLanguage extends Language {
    public static final RavenLanguage INSTANCE = new RavenLanguage();

    private RavenLanguage() {
        super("Raven");
    }
}
