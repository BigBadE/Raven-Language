package bigbade.raven.ravenintellijplugin;

import com.intellij.openapi.fileTypes.LanguageFileType;
import com.intellij.openapi.util.NlsContexts;
import com.intellij.openapi.util.NlsSafe;
import org.jetbrains.annotations.NonNls;
import org.jetbrains.annotations.NotNull;

import javax.swing.Icon;

public class RavenFileType extends LanguageFileType {
    public static final RavenFileType INSTANCE = new RavenFileType();

    private RavenFileType() {
        super(RavenLanguage.INSTANCE);
    }

    @Override
    @NonNls
    @NotNull
    public String getName() {
        return "Raven File";
    }

    @Override
    @NlsContexts.Label
    @NotNull
    public String getDescription() {
        return "Raven Source File";
    }

    @Override
    @NlsSafe
    @NotNull
    public String getDefaultExtension() {
        return "rv";
    }

    @Override
    public Icon getIcon() {
        return RavenIcon.FILE;
    }
}
