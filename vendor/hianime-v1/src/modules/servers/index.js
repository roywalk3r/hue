/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import serversSchema from './servers.schema.js';
import serversHandler from './servers.handler.js';

export { serversHandler as handler, serversSchema as schema };
