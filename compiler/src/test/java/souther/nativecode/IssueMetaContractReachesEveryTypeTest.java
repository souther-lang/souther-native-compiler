package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * What every host's test of its issue metadata is held to has an issue of each type the library
 * writes, so a type added to the metadata without a document that reaches it fails here, before a
 * host could take it for one it never sees.
 */
class IssueMetaContractReachesEveryTypeTest {

    @Test
    void theJvmsAnswersHoldAValueOfEveryTypeTheMetadataWrites() throws IOException {
        assertThat(IssueMetaContract.typesIn(IssueMetaContract.expected()))
                .isEqualTo(IssueMetaContract.metaTypes());
    }
}
