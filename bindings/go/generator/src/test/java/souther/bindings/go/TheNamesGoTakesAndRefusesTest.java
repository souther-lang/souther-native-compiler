package souther.bindings.go;

import org.junit.jupiter.api.Test;
import souther.bindings.NotBindable;

import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/** What a binding is called and what its members are called, as Go takes them, and what it refuses. */
class TheNamesGoTakesAndRefusesTest {

    @Test
    void anImportPathIsOneAnotherModuleCanRequire() {
        assertThat(GoNames.importPath("example.com/acme")).isEqualTo("example.com/acme");
        assertThat(GoNames.importPath("example.com/a-b/c_d~e")).isEqualTo("example.com/a-b/c_d~e");
        for (String refused : List.of("acme", "", "/example.com/x", "example.com/x/", "example.com//x",
                "example.com/../x", "example.com/./x", "example.com/a b", "example.com/.hidden",
                "example.com/1x", "example.com/main", "example.com/type", "example.com/é")) {
            assertThatThrownBy(() -> GoNames.importPath(refused)).as(refused)
                    .isInstanceOf(NotBindable.class);
        }
    }

    @Test
    void aPackageIsNamedByTheLastPartAndNotByAMajorVersion() {
        assertThat(GoNames.packageName("example.com/acme")).isEqualTo("acme");
        assertThat(GoNames.packageName("example.com/a-b")).isEqualTo("a_b");
        assertThat(GoNames.packageName("example.com/a.b")).isEqualTo("a_b");
        assertThat(GoNames.packageName("example.com/acme/v2")).isEqualTo("acme");
    }

    @Test
    void aModuleIsAPackageOfEachPartAndRefusesWhatGoOrTheToolsReadAsMore() {
        assertThat(GoNames.modulePath("cart.lines")).containsExactly("cart", "lines");
        for (String refused : List.of("internal.x", "x.vendor", "x.testdata", "x.main", "x.type",
                "_x.y", "x._y", "x.1y", "x.é")) {
            assertThatThrownBy(() -> GoNames.modulePath(refused)).as(refused)
                    .isInstanceOf(NotBindable.class);
        }
    }

    @Test
    void aTypeIsExportedByItsFirstLetterAndACaseNameWithNoCapitalIsRefused() {
        assertThat(GoNames.exported("price", "a type")).isEqualTo("Price");
        assertThat(GoNames.exported("Price", "a type")).isEqualTo("Price");
        for (String refused : List.of("_x", "1a", "c", "数量", "")) {
            assertThatThrownBy(() -> GoNames.exported(refused, "a type")).as(refused)
                    .isInstanceOf(NotBindable.class);
        }
    }

    @Test
    void aParameterIsAsTheModelNamesItUnlessItMeansSomethingInGo() {
        for (String same : List.of("count0", "err", "fn", "answer", "sku", "M0", "mm", "m")) {
            assertThat(GoNames.local(same, "a parameter")).isEqualTo(same);
        }
        for (String escaped : List.of("type", "r", "v", "m0", "m12", "len", "nil", "string", "souther",
                "lib", "raoh", "unsafe", "C", "any", "make")) {
            assertThat(GoNames.local(escaped, "a parameter")).isEqualTo(escaped + "_");
        }
    }

    @Test
    void theNamesAFileGivesWhatItImportsAreReservedByTheOneRuleThatMakesThem() {
        Body.Imports imports = new Body.Imports("example.com/x", "example.com/x/here");
        for (int at = 0; at < 12; at++) {
            String alias = imports.module("example.com/x/there" + at).replace(".", "");
            assertThat(GoNames.reserved(alias)).as(alias).isTrue();
        }
    }
}
