# Deploying to Vercel

This guide explains how to deploy your Express API to Vercel.

## Prerequisites

1. Install Vercel CLI globally:
```bash
npm install -g vercel
```

2. Sign up for a Vercel account at https://vercel.com

## Environment Variables

Before deploying, you need to configure your environment variables in Vercel. Your app requires:

- `TMDB_API_KEY` - Your TMDB API key
- `FEBBOX_COOKIES` - Cookies for Febbox provider (optional)
- `AUTH_PASSWORD` - Password for authentication (optional)
- Any other custom environment variables from your `.env` file

### Setting Environment Variables

**Via Vercel CLI:**
```bash
vercel env add TMDB_API_KEY
vercel env add FEBBOX_COOKIES
vercel env add AUTH_PASSWORD
```

**Via Vercel Dashboard:**
1. Go to your project settings
2. Navigate to "Environment Variables"
3. Add each variable for Production, Preview, and Development environments

## Deployment Methods

### Method 1: Deploy with Vercel CLI (Recommended)

1. **Login to Vercel:**
```bash
vercel login
```

2. **Deploy to preview:**
```bash
vercel
```

3. **Deploy to production:**
```bash
vercel --prod
```

### Method 2: Deploy via Git Integration

1. Push your code to GitHub, GitLab, or Bitbucket
2. Go to https://vercel.com/new
3. Import your repository
4. Configure environment variables in the project settings
5. Deploy

## Important Notes

### Vercel Function Limitations

Your Express app runs as a single Vercel Function with these limitations:

1. **Max Bundle Size:** 250MB (including dependencies)
2. **Execution Timeout:** 10 seconds (Hobby), 60 seconds (Pro)
3. **Memory:** 1024MB default
4. **Cold Starts:** First request may be slower

### Static Files

- Place static files in the `public/` directory
- They'll be served via Vercel's CDN
- `express.static()` won't work on Vercel

### Local Development

Your app will run normally in local development:
```bash
npm start
# or
npm run start:dev
```

The server only starts when NOT in Vercel environment, so the same code works both locally and on Vercel.

## Configuration Files

### vercel.json
Routes all requests to your Express app and sets production environment.

### .vercelignore
Excludes unnecessary files from deployment to reduce bundle size.

## Testing Your Deployment

After deployment, test your endpoints:

```bash
# Replace YOUR_DEPLOYMENT_URL with your actual Vercel URL
curl https://YOUR_DEPLOYMENT_URL.vercel.app/api/health
curl https://YOUR_DEPLOYMENT_URL.vercel.app/api/providers
```

## Troubleshooting

### Build Fails
- Check build logs in Vercel dashboard
- Ensure all dependencies are in `package.json`
- Verify Node version compatibility

### Function Timeout
- Optimize slow providers
- Consider caching responses
- Upgrade to Pro plan for 60s timeout

### Environment Variables Not Working
- Redeploy after adding variables
- Check variable names match exactly
- Ensure variables are set for the correct environment

## Monitoring

Monitor your deployment:
- **Logs:** Available in Vercel dashboard
- **Analytics:** Track function invocations and errors
- **Metrics:** Check response times and usage

## Custom Domain

To add a custom domain:
1. Go to project settings in Vercel dashboard
2. Navigate to "Domains"
3. Add your domain and configure DNS as instructed

## Resources

- [Vercel Express Documentation](https://vercel.com/docs/frameworks/backend/express)
- [Vercel Functions Limitations](https://vercel.com/docs/functions/limitations)
- [Vercel CLI Documentation](https://vercel.com/docs/cli)
