package bigbade.raven.ravenintellijplugin.parsing;

public interface RavenTokenSets {
    RavenElementType[] ALL_TYPES = new RavenElementType[]{RavenTypes.Start,
            RavenTypes.EOF,
            RavenTypes.InvalidCharacters,
            RavenTypes.StringStart,
            RavenTypes.StringEscape,
            RavenTypes.StringEnd,
            RavenTypes.ImportStart,
            RavenTypes.Identifier,
            RavenTypes.AttributesStart,
            RavenTypes.Attribute,
            RavenTypes.ModifiersStart,
            RavenTypes.Modifier,
            RavenTypes.ElemStart,
            RavenTypes.GenericsStart,
            RavenTypes.Generic,
            RavenTypes.GenericBound,
            RavenTypes.GenericEnd,
            RavenTypes.ArgumentsStart,
            RavenTypes.ArgumentName,
            RavenTypes.ArgumentType,
            RavenTypes.ArgumentEnd,
            RavenTypes.ArgumentsEnd,
            RavenTypes.ReturnType,
            RavenTypes.CodeStart
    };
}
