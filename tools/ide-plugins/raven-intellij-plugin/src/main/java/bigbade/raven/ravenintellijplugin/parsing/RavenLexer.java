package bigbade.raven.ravenintellijplugin.parsing;

import bigbade.raven.ravenintellijplugin.natives.NativeParserRunner;
import bigbade.raven.ravenintellijplugin.natives.NativeUtils;
import com.intellij.lexer.Lexer;
import com.intellij.lexer.LexerPosition;
import com.intellij.psi.tree.IElementType;
import org.bouncycastle.math.raw.Nat;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

import java.lang.annotation.Native;

public class RavenLexer extends Lexer {
    public String buffer;
    public long reference;
    public int start;
    public int end;

    @Override
    public void start(@NotNull CharSequence inBuffer, int startOffset, int endOffset, int initialState) {
        System.out.println("Test: " + inBuffer + ", " + startOffset + ", " + endOffset + ", " + initialState);
        NativeUtils.setup();
        buffer = inBuffer.toString();
        start = startOffset;
        end = endOffset;
        reference = NativeParserRunner.start(buffer, startOffset, endOffset, initialState);
    }

    @Override
    public int getState() {
        System.out.println("Test 2");
        return NativeParserRunner.getState(reference);
    }

    @Override
    public @Nullable IElementType getTokenType() {
        System.out.println("Test 3: " + reference);
        return RavenTokenSets.ALL_TYPES[NativeParserRunner.getTokenType(reference)];
    }

    @Override
    public int getTokenStart() {
        System.out.println("Test 4: " + reference);
        int output = NativeParserRunner.getTokenStart(reference);
        System.out.println("Got output: " + output);
        return output;
    }

    @Override
    public int getTokenEnd() {
        System.out.println("Test 5");
        return NativeParserRunner.getTokenEnd(reference);
    }

    @Override
    public void advance() {
        System.out.println("Test 6");
        NativeParserRunner.advance(reference);
    }

    @Override
    public @NotNull LexerPosition getCurrentPosition() {
        System.out.println("Test 7");
        return new RavenLexerPosition(NativeParserRunner.getCurrentPosition(reference));
    }

    @Override
    public void restore(@NotNull LexerPosition position) {
        System.out.println("Test 8");
        NativeParserRunner.restore(((RavenLexerPosition) position).getId(), reference);
    }

    @Override
    public @NotNull CharSequence getBufferSequence() {
        System.out.println("Test 9");
        return buffer;
    }

    @Override
    public int getBufferEnd() {
        System.out.println("Test 10");
        return end;
    }

    public static class RavenLexerPosition implements LexerPosition {
        private long id;

        public RavenLexerPosition(long foundId) {
            id = foundId;
        }

        public long getId() {
            return id;
        }

        @Override
        public int getOffset() {
            return NativeParserRunner.getPositionOffset(id);
        }

        @Override
        public int getState() {
            return NativeParserRunner.getPositionState(id);
        }
    }
}
