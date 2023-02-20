package bigbade.raven.ravenintellijplugin.parsing;

import bigbade.raven.ravenintellijplugin.RavenFileType;
import bigbade.raven.ravenintellijplugin.RavenLanguage;
import com.intellij.extapi.psi.PsiFileBase;
import com.intellij.openapi.fileTypes.FileType;
import com.intellij.psi.FileViewProvider;
import org.jetbrains.annotations.NotNull;

public class RavenFile extends PsiFileBase {
    protected RavenFile(@NotNull FileViewProvider viewProvider) {
        super(viewProvider, RavenLanguage.INSTANCE);
    }

    @Override
    @NotNull
    public FileType getFileType() {
        return RavenFileType.INSTANCE;
    }

    @Override
    public String toString() {
        return "Raven File";
    }
}
