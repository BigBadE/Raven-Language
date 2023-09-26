package bigbade.raven.ravenintellijplugin.parsing;

import com.intellij.lang.ASTNode;
import com.intellij.lang.PsiBuilder;
import com.intellij.lang.PsiParser;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NotNull;

public class RavenParser implements PsiParser {
    @Override
    public @NotNull ASTNode parse(@NotNull IElementType root, @NotNull PsiBuilder builder) {
        throw new RuntimeException("Test!");
        /*builder.mark().done(RavenTypes.EOF);
        return builder.getTreeBuilt();*/
    }
}
