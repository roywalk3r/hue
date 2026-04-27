/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

const hitvApi = require('./utils/hitv-api');

async function main() {
    console.log('Testing getHero()...');
    const hero = await hitvApi.getHero();
    console.log(`Hero count: ${hero.length}`);
    if (hero.length > 0) {
        console.log('First item:', JSON.stringify(hero[0], null, 2));
    } else {
        console.log('Hero is empty!');
    }
}

main();