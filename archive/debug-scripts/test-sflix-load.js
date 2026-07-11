/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

try {
    const sflix = require('./providers/sflix.js');
    console.log('SFlix loaded successfully');
} catch (e) {
    console.error('SFlix load failed:', e.message);
    console.error(e.stack);
}
