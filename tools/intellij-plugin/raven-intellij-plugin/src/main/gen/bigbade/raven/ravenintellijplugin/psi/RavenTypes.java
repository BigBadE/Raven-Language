// This is a generated file. Not intended for manual editing.
package bigbade.raven.ravenintellijplugin.psi;

import com.intellij.psi.tree.IElementType;
import com.intellij.psi.PsiElement;
import com.intellij.lang.ASTNode;
import bigbade.raven.ravenintellijplugin.parsing.RavenElementType;
import bigbade.raven.ravenintellijplugin.parsing.RavenTokenType;
import bigbade.raven.ravenintellijplugin.psi.impl.*;

public interface RavenTypes {

  IElementType PROPERTY = new RavenElementType("PROPERTY");

  IElementType COMMENT = new RavenTokenType("COMMENT");
  IElementType CRLF = new RavenTokenType("CRLF");
  IElementType KEY = new RavenTokenType("KEY");
  IElementType SEPARATOR = new RavenTokenType("SEPARATOR");
  IElementType VALUE = new RavenTokenType("VALUE");

  class Factory {
    public static PsiElement createElement(ASTNode node) {
      IElementType type = node.getElementType();
      if (type == PROPERTY) {
        return new RavenPropertyImpl(node);
      }
      throw new AssertionError("Unknown element type: " + type);
    }
  }
}
