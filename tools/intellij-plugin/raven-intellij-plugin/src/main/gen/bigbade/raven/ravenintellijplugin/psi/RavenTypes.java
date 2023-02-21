// This is a generated file. Not intended for manual editing.
package bigbade.raven.ravenintellijplugin.psi;

import com.intellij.psi.tree.IElementType;
import com.intellij.psi.PsiElement;
import com.intellij.lang.ASTNode;
import bigbade.raven.ravenintellijplugin.parsing.RavenElementType;
import bigbade.raven.ravenintellijplugin.parsing.RavenTokenType;
import bigbade.raven.ravenintellijplugin.psi.impl.*;

public interface RavenTypes {

  IElementType MODIFIER = new RavenElementType("MODIFIER");
  IElementType STRUCTURE = new RavenElementType("STRUCTURE");

  IElementType FUNCTION = new RavenTokenType("function");
  IElementType STRUCT = new RavenTokenType("struct");

  class Factory {
    public static PsiElement createElement(ASTNode node) {
      IElementType type = node.getElementType();
      if (type == MODIFIER) {
        return new RavenModifierImpl(node);
      }
      else if (type == STRUCTURE) {
        return new RavenStructureImpl(node);
      }
      throw new AssertionError("Unknown element type: " + type);
    }
  }
}
