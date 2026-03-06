const path = require( 'path' );
const MiniCssExtractPlugin = require( 'mini-css-extract-plugin' );
const CssMinimizerPlugin = require( 'css-minimizer-webpack-plugin' );
const TerserPlugin = require( 'terser-webpack-plugin' );

module.exports = {
	mode: 'production',
	entry: {
		editor: path.resolve( __dirname, 'src/editor/index.js' ),
	},
	output: {
		path: path.resolve( __dirname, 'static/js' ),
		filename: '[name].js',
		clean: true,
	},
	resolve: {
		extensions: [ '.js', '.jsx', '.ts', '.tsx', '.json', '.mjs' ],
		// Some @wordpress packages import without extensions
		fullySpecified: false,
	},
	module: {
		rules: [
			// Disable fullySpecified for .mjs files (ESM compat)
			{
				test: /\.m?js$/,
				resolve: {
					fullySpecified: false,
				},
			},
			{
				test: /\.m?jsx?$/,
				exclude: /node_modules\/(?!@wordpress)/,
				use: {
					loader: require.resolve( 'babel-loader' ),
					options: {
						presets: [
							require.resolve( '@babel/preset-env' ),
							require.resolve( '@babel/preset-react' ),
						],
					},
				},
			},
			{
				test: /\.css$/,
				use: [
					MiniCssExtractPlugin.loader,
					'css-loader',
				],
			},
			{
				test: /\.(png|jpg|gif|svg|woff|woff2|eot|ttf)$/,
				type: 'asset/inline',
			},
		],
	},
	plugins: [
		new MiniCssExtractPlugin( {
			filename: '[name].css',
		} ),
	],
	optimization: {
		minimizer: [
			new TerserPlugin(),
			new CssMinimizerPlugin(),
		],
	},
	// Do NOT externalize @wordpress packages - bundle everything
	externals: {},
	performance: {
		maxAssetSize: 10 * 1024 * 1024,
		maxEntrypointSize: 10 * 1024 * 1024,
	},
};
