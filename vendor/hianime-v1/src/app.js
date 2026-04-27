/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

import createApp from './lib/create-app.js';
import { configureDocs } from './lib/configure-docs.js';
import router from './routes/routes.js';

const app = createApp();

configureDocs(app);

app.route('/api/v1', router);

export default app;
