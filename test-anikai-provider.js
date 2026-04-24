const anikai = require('./providers/anikai');

(async () => {
    try {
        console.log('Testing Anikai Provider...');
        
        // One Piece Episode 1
        const slug = 'one-piece-dk6r';
        const ep = 1;
        
        console.log(`Getting stream for ${slug} ep ${ep}...`);
        const result = await anikai.getAnimeStream(slug, ep);
        
        if (result) {
            console.log('--- TEST RESULT ---');
            console.log(JSON.stringify(result, null, 2));
            
            if (result.sources) {
                console.log('SUCCESS: Decrypted sources found!');
            } else if (result.iframe) {
                console.log('FALLBACK: Iframe captured!');
            }
        } else {
            console.log('FAILED: No result returned');
        }
        
    } catch (err) {
        console.error('TEST ERROR:', err);
    } finally {
        process.exit();
    }
})();