const { getStreams } = require('./providers/sflix');

async function test() {
    console.log("Testing SFlix Flow (API + Subtitles)...");
    try {
        // Test with a known movie: Avengers: Infinity War (TMDB ID: 299536)
        console.log("\n--- Movie Test: Avengers: Infinity War ---");
        const movieStreams = await getStreams(299536, 'movie');
        console.log(`Found ${movieStreams.length} streams.`);
        if (movieStreams.length > 0) {
            const s = movieStreams[0];
            console.log(`Sample Stream: ${s.title} | Format: ${s.format} | Subs: ${s.subtitles ? s.subtitles.length : 0}`);
            if (s.subtitles && s.subtitles.length > 0) {
                console.log(`First Sub: ${s.subtitles[0].label} (${s.subtitles[0].lang})`);
            }
        }

        // Test with a known TV show: The Flash (TMDB ID: 60735)
        console.log("\n--- TV Test: The Flash S1 E1 ---");
        const tvStreams = await getStreams(60735, 'tv', 1, 1);
        console.log(`Found ${tvStreams.length} streams.`);
        if (tvStreams.length > 0) {
            const s = tvStreams[0];
            console.log(`Sample Stream: ${s.title} | Format: ${s.format} | Subs: ${s.subtitles ? s.subtitles.length : 0}`);
            if (s.subtitles && s.subtitles.length > 0) {
                console.log(`First Sub: ${s.subtitles[0].label} (${s.subtitles[0].lang})`);
            }
        }

    } catch (e) {
        console.error("Test failed:", e);
    }
}

test();
