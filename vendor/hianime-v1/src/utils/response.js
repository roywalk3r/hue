/**
 * Hue Stream API
 * Professional Streaming Aggregator
 *
 * @author roywalk3r <roywalker440@gmail.com>
 * @repository https://github.com/roywalk3r/hue.git
 * @license MIT
 */

export const success = (c, data, statusCode = 200) => {
  return c.json({ success: true, data }, { status: statusCode });
};

export const fail = (c, message = 'internal server error', statusCode = 500, details = null) => {
  return c.json({ success: false, message, details }, { status: statusCode });
};
