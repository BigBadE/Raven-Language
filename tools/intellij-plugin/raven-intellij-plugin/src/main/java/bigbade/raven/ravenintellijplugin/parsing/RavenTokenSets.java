package bigbade.raven.ravenintellijplugin.parsing;

import bigbade.raven.ravenintellijplugin.psi.RavenTypes;
import com.intellij.psi.tree.TokenSet;

public interface RavenTokenSets {
    TokenSet IDENTIFIERS = TokenSet.create(RavenTypes.KEY);

    TokenSet COMMENT = TokenSet.create(RavenTypes.COMMENT);
}
