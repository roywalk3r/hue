/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

export default function createEndpoint(path, page) {
  if (page === 1) {
    return `/${path}`;
  } else {
    return path.includes('?') ? `/${path}&page=${page}` : `/${path}?page=${page}`;
  }
}
