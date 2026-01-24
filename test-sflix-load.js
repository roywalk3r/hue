try {
    const sflix = require('./providers/sflix.js');
    console.log('SFlix loaded successfully');
} catch (e) {
    console.error('SFlix load failed:', e.message);
    console.error(e.stack);
}
