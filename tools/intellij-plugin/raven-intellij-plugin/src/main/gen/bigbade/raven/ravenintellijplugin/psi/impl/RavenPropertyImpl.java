// This is a generated file. Not intended for manual editing.
package bigbade.raven.ravenintellijplugin.psi.impl;

import java.util.List;
import org.jetbrains.annotations.*;
import com.intellij.lang.ASTNode;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiElementVisitor;
import com.intellij.psi.util.PsiTreeUtil;
import static bigbade.raven.ravenintellijplugin.psi.RavenTypes.*;
import com.intellij.extapi.psi.ASTWrapperPsiElement;
import bigbade.raven.ravenintellijplugin.psi.*;

public class RavenPropertyImpl extends ASTWrapperPsiElement implements RavenProperty {

  public RavenPropertyImpl(@NotNull ASTNode node) {
    super(node);
  }

  public void accept(@NotNull RavenVisitor visitor) {
    visitor.visitProperty(this);
  }

  @Override
  public void accept(@NotNull PsiElementVisitor visitor) {
    if (visitor instanceof RavenVisitor) accept((RavenVisitor)visitor);
    else super.accept(visitor);
  }

}
