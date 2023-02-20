package bigbade.raven.ravenintellijplugin.parsing;

import com.intellij.lexer.FlexAdapter;

public class RavenLexerAdapter extends FlexAdapter {
    public RavenLexerAdapter() {
        super(new RavenLexer(null));
    }
}
