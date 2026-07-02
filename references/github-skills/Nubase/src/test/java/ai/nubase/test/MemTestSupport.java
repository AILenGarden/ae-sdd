package ai.nubase.test;

import org.springframework.beans.factory.ObjectProvider;

import java.util.Iterator;
import java.util.stream.Stream;

public final class MemTestSupport {

    private MemTestSupport() {
    }

    public static <T> ObjectProvider<T> emptyProvider() {
        return new ObjectProvider<>() {
            @Override
            public T getObject(Object... args) {
                return null;
            }

            @Override
            public T getIfAvailable() {
                return null;
            }

            @Override
            public T getIfUnique() {
                return null;
            }

            @Override
            public T getObject() {
                return null;
            }

            @Override
            public Iterator<T> iterator() {
                return Stream.<T>empty().iterator();
            }
        };
    }
}
