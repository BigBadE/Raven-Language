package bigbade.raven.ravenintellijplugin.parsing;

import bigbade.raven.ravenintellijplugin.RavenLanguage;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NonNls;
import org.jetbrains.annotations.NotNull;

public class RavenElementType extends IElementType {
    public RavenElementType(@NonNls @NotNull String debugName) {
        super(debugName, RavenLanguage.INSTANCE);
    }
}
