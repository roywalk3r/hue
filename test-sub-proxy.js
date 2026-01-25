const fetch = require('node-fetch');

(async () => {
    const subUrl = "https://sub.streamsub.top/The-Judge-from-Hell.Ep1_eng.srt?v=09";
    const proxyUrl = `http://localhost:24608/sub-proxy?url=${encodeURIComponent(subUrl)}`;
    
    console.log(`Testing Subtitle Proxy for: ${subUrl}`);
    console.log(`Calling Proxy: ${proxyUrl}`);
    
    try {
        const res = await fetch(proxyUrl);
        console.log(`Status: ${res.status}`);
        console.log(`Content-Type: ${res.headers.get('content-type')}`);
        
        if (res.ok) {
            const text = await res.text();
            console.log("\n--- Subtitle Content Snippet ---");
            console.log(text.substring(0, 200));
            console.log("------------------------------");
            console.log("SUCCESS: Proxy fetched content.");
        } else {
            console.log(`FAILURE: Proxy returned ${res.status}`);
        }
    } catch (e) {
        console.error("Test Error:", e.message);
    }
})();

