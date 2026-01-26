const { getStreams } = require('./providers/sflix');

async function test() {
    console.log("Testing SFlix API Search...");
    try {
        // Test with a known movie
        console.log("\n--- Movie Search Test ---");
        const movieStreams = await getStreams(299536, 'movie'); // Avengers: Infinity War
        console.log("Movie Streams:", movieStreams ? movieStreams.length : 0);
        if (movieStreams && movieStreams.length > 0) {
            console.log("First stream:", movieStreams[0]);
        }

        // Test with a known TV show
        console.log("\n--- TV Show Search Test ---");
        const tvStreams = await getStreams(60735, 'tv', 1, 1); // The Flash
        console.log("TV Streams:", tvStreams ? tvStreams.length : 0);
        if (tvStreams && tvStreams.length > 0) {
            console.log("First stream:", tvStreams[0]);
        }

    } catch (e) {
        console.error("Test failed:", e);
    }
}

test();
