package bigbade.raven.ravenintellijplugin.parsing;

import com.intellij.extapi.psi.ASTWrapperPsiElement;
import com.intellij.lang.ASTNode;
import com.intellij.psi.PsiElement;
import com.intellij.psi.tree.IElementType;
import org.intellij.lang.annotations.Identifier;

public interface RavenTypes {
    public static RavenElementType Start = new RavenElementType("Start");
    public static RavenElementType EOF = new RavenElementType("EOF");
    public static RavenElementType InvalidCharacters = new RavenElementType("InvalidCharacters");
    public static RavenElementType StringStart = new RavenElementType("StringStart");
    public static RavenElementType StringEscape = new RavenElementType("StringEscape");
    public static RavenElementType StringEnd = new RavenElementType("StringEnd");
    public static RavenElementType ImportStart = new RavenElementType("ImportStart");
    public static RavenElementType Identifier = new RavenElementType("Identifier");
    public static RavenElementType AttributesStart = new RavenElementType("AttributesStart");
    public static RavenElementType Attribute = new RavenElementType("Attribute");
    public static RavenElementType ModifiersStart = new RavenElementType("ModifiersStart");
    public static RavenElementType Modifier = new RavenElementType("Modifier");
    public static RavenElementType ElemStart = new RavenElementType("ElemStart");
    public static RavenElementType GenericsStart = new RavenElementType("GenericsStart");
    public static RavenElementType Generic = new RavenElementType("Generic");
    public static RavenElementType GenericBound = new RavenElementType("GenericBound");
    public static RavenElementType GenericEnd = new RavenElementType("GenericEnd");
    public static RavenElementType ArgumentsStart = new RavenElementType("ArgumentsStart");
    public static RavenElementType ArgumentName = new RavenElementType("ArgumentName");
    public static RavenElementType ArgumentType = new RavenElementType("ArgumentType");
    public static RavenElementType ArgumentEnd = new RavenElementType("ArgumentEnd");
    public static RavenElementType ArgumentsEnd = new RavenElementType("ArgumentsEnd");
    public static RavenElementType ReturnType = new RavenElementType("ReturnType");
    public static RavenElementType CodeStart = new RavenElementType("CodeStart");


    class Factory {
        public static PsiElement createElement(ASTNode node) {
            return new ASTWrapperPsiElement(node);
        }
    }
}
