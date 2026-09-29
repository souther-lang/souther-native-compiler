package souther.nativecode;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;

/** How what a command fetches is read from where it is: the bytes at an address, or why not. */
interface Downloads {

    byte[] get(URI address) throws IOException;

    /** Over HTTP, following the redirects a release asset is served through. */
    static Downloads http() {
        HttpClient client = HttpClient.newBuilder()
                .followRedirects(HttpClient.Redirect.NORMAL)
                .connectTimeout(Duration.ofSeconds(15))
                .build();
        return address -> {
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
