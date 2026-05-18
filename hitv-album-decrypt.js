/**
 * HUE ENGINE ❄
 * System Designation: The Architecture of Aggregation
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

async (page) => {
  const did = await page.evaluate(() => {
    const m = document.cookie.match(/hpwa_did=([^;]+)/);
    return m ? m[1] : '';
  });
  
  const response = await page.evaluate(async () => {
    const res = await fetch('/s1/w/routine/api/seriesAlbum/detail?aid=a_sSaCsLFo1e_412f0cCSn&page=1');
    return await res.text();
  });
  
  const json = JSON.parse(response);
  
  return JSON.stringify({ did, ts: json.ts, rescode: json.rescode, dataLength: json.data ? json.data.length : 0, dataPreview: json.data ? json.data.substring(0, 80) : null });
}
