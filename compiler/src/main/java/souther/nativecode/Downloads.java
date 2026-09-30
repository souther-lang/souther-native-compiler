package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;

/**
 * How what a command fetches is read from where it is: the bytes at an address, or why not.
 *
 * <p>An address is {@code http:}, {@code https:} or {@code file:}. A {@code file:} one reads a
 * directory laid out as the repository or the release it stands for, so that a release rehearsed from
 * what a build made on disk is asked for at the same paths, and held to the same checksums, as one
 * published.
 */
interface Downloads {

    byte[] get(URI address) throws IOException;

    /** Over HTTP, following the redirects a release asset is served through. */
    static Downloads http() {
        HttpClient client = HttpClient.newBuilder()
                .followRedirects(HttpClient.Redirect.NORMAL)
                .connectTimeout(Duration.ofSeconds(15))
                .build();
        return address -> {
            if ("file".equals(address.getScheme())) {
                try {
                    return java.nio.file.Files.readAllBytes(java.nio.file.Path.of(address));
                } catch (java.nio.file.NoSuchFileException e) {
                    throw new IOException(address + " is not there", e);
                }
            }
            try {
                HttpResponse<byte[]> answer = client.send(
                        HttpRequest.newBuilder(address).timeout(Duration.ofMinutes(5)).GET().build(),
                        HttpResponse.BodyHandlers.ofByteArray());
                if (answer.statusCode() != 200) {
                    throw new IOException(address + " answered " + answer.statusCode());
                }
                return answer.body();
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new IOException("interrupted while fetching " + address, e);
            }
        };
    }
}
