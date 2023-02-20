package bigbade.raven.ravenintellijplugin.parsing;

import bigbade.raven.ravenintellijplugin.RavenLanguage;
import com.intellij.lang.Language;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NonNls;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

public class RavenTokenType extends IElementType {
    public RavenTokenType(@NonNls @NotNull String debugName) {
        super(debugName, RavenLanguage.INSTANCE);
    }

    @Override
    public String toString() {
        return "RavenTokenType." + super.toString();
    }
}
