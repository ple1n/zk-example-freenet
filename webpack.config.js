
        const path = require("path");

        module.exports = {
        entry: "./src/index.ts",
        devtool: "inline-source-map",
        output: {
            filename: "bundle.js",
            path: path.resolve(__dirname, "dist"),
        },
        resolve: {
            extensions: [".tsx", ".ts", ".js"],
        },
        devServer: {
            static: path.resolve(__dirname, "dist"),
            port: 8080,
            hot: true,
        },
        module: {
            rules: [
                {
                    test: /\.tsx?$/,
                    use: "ts-loader",
                    exclude: /node_modules/,
                }
            ],
        },
        };