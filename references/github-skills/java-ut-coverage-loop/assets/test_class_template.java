package {{PACKAGE}};

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

@ExtendWith(MockitoExtension.class)
class {{SUT_SIMPLE}}Test {

    // TODO: replace with actual collaborator types from {{SUT_SIMPLE}}'s constructor / setters
    // @Mock private SomeCollaborator someCollaborator;

    @InjectMocks private {{SUT_SIMPLE}} {{SUT_VAR}};

    @BeforeEach
    void setUp() {
        // shared arrange phase, only when reused across most tests
    }

    @Test
    void should_REPLACE_outcome_when_REPLACE_condition() {
        // given
        // ... set up fixtures + when(collaborator.x()).thenReturn(...)

        // when
        // var result = {{SUT_VAR}}.methodUnderTest(...);

        // then
        // assertThat(result).isEqualTo(...);
        // verify(collaborator).y(eq(...));
    }
}
